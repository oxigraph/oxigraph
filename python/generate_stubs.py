import argparse
import ast
import importlib
import inspect
import logging
import re
import subprocess
from functools import reduce
from typing import Set, List, Mapping, Any

AST_LOAD = ast.Load()
AST_ELLIPSIS = ast.Ellipsis()
AST_STORE = ast.Store()
AST_TYPING_ANY = ast.Attribute(
    value=ast.Name(id="typing", ctx=AST_LOAD), attr="Any", ctx=AST_LOAD
)
GENERICS = {
    "iter": ast.Attribute(
        value=ast.Name(id="typing", ctx=AST_LOAD), attr="Iterator", ctx=AST_LOAD
    ),
    "list": ast.Attribute(
        value=ast.Name(id="typing", ctx=AST_LOAD), attr="List", ctx=AST_LOAD
    ),
}
OBJECT_MEMBERS = dict(inspect.getmembers(object))


ATTRIBUTES_BLACKLIST = {
    "__class__",
    "__dir__",
    "__doc__",
    "__init_subclass__",
    "__module__",
    "__new__",
    "__subclasshook__",
}


def module_stubs(module) -> ast.Module:
    types_to_import = {"typing"}
    classes = []
    functions = []
    for (member_name, member_value) in inspect.getmembers(module):
        if member_name.startswith("__"):
            pass
        elif inspect.isclass(member_value):
            classes.append(class_stubs(member_name, member_value, types_to_import))
        elif inspect.isbuiltin(member_value):
            functions.append(function_stub(member_name, member_value, types_to_import))
        else:
            logging.warning(f"Unsupported root construction {member_name}")
    return ast.Module(
        body=[ast.Import(names=[ast.alias(name=t)]) for t in sorted(types_to_import)]
        + classes
        + functions,
        type_ignores=[],
    )


def class_stubs(cls_name: str, cls_def, types_to_import: Set[str]) -> ast.ClassDef:
    attributes: List[ast.AST] = []
    methods: List[ast.AST] = []
    magic_methods: List[ast.AST] = []
    for (member_name, member_value) in inspect.getmembers(cls_def):
        if member_name == "__init__":
            try:
                inspect.signature(cls_def)  # we check it actually exists
                methods = [
                    function_stub(member_name, cls_def, types_to_import)
                ] + methods
            except ValueError as e:
                if "no signature found" not in str(e):
                    raise ValueError(
                        f"Error while parsing signature of {cls_name}.__init__: {e}"
                    )
        elif member_name in ATTRIBUTES_BLACKLIST or member_value == OBJECT_MEMBERS.get(
            member_name
        ):
            pass
        elif inspect.isdatadescriptor(member_value):
            attributes.extend(
                data_descriptor_stub(member_name, member_value, types_to_import)
            )
        elif inspect.isroutine(member_value):
            (magic_methods if member_name.startswith("__") else methods).append(
                function_stub(member_name, member_value, types_to_import)
            )
        else:
            logging.warning(f"Unsupported member {member_name} of class {cls_name}")

    doc = inspect.getdoc(cls_def)
    return ast.ClassDef(
        cls_name,
        bases=[],
        keywords=[],
        body=(
            ([build_doc_comment(doc)] if doc else [])
            + attributes
            + methods
            + magic_methods
        )
        or [AST_ELLIPSIS],
        decorator_list=[
            ast.Attribute(
                value=ast.Name(id="typing", ctx=AST_LOAD), attr="final", ctx=AST_LOAD
            )
        ],
    )


def data_descriptor_stub(
    data_desc_name: str, data_desc_def, types_to_import: Set[str]
) -> tuple:
    annotation = None
    doc_comment = None

    doc = inspect.getdoc(data_desc_def)
    if doc is not None:
        annotation = returns_stub(doc, types_to_import)
        m = re.findall(r":return: *(.*) *\n", doc)
        if len(m) == 1:
            doc_comment = m[0]
        elif len(m) > 1:
            raise ValueError("Multiple return annotations found with :return:")

    assign = ast.AnnAssign(
        target=ast.Name(id=data_desc_name, ctx=AST_STORE),
        annotation=annotation or AST_TYPING_ANY,
        simple=1,
    )
    return (assign, build_doc_comment(doc_comment)) if doc_comment else (assign,)


def function_stub(fn_name: str, fn_def, types_to_import: Set[str]) -> ast.FunctionDef:
    body = []
    doc = inspect.getdoc(fn_def)
    if doc is not None and not fn_name.startswith("__"):
        body.append(build_doc_comment(doc))

    return ast.FunctionDef(
        fn_name,
        arguments_stub(fn_name, fn_def, doc or "", types_to_import),
        body or [AST_ELLIPSIS],
        decorator_list=[],
        returns=returns_stub(doc, types_to_import) if doc else None,
        lineno=0,
    )


def arguments_stub(callable_name, callable_def, doc: str, types_to_import: Set[str]):
    real_parameters: Mapping[str, inspect.Parameter] = inspect.signature(
        callable_def
    ).parameters
    if callable_name == "__init__":
        real_parameters = {
            "self": inspect.Parameter("self", inspect.Parameter.POSITIONAL_ONLY),
            **real_parameters,
        }

    parsed_param_types = {}
    optional_params = set()
    for match in re.findall(r"\n *:type *([a-z_]+): ([^\n]*) *\n", doc):
        if match[0] not in real_parameters:
            raise ValueError(
                f"The parameter {match[0]} is defined in the documentation but not in the function signature"
            )
        type = match[1]
        if type.endswith(", optional"):
            optional_params.add(match[0])
            type = type[:-10]
        parsed_param_types[match[0]] = convert_type_from_doc(type, types_to_import)

    # we parse the parameters
    posonlyargs = []
    args = []
    vararg = None
    kwonlyargs = []
    kw_defaults = []
    kwarg = None
    defaults = []
    for param in real_parameters.values():
        if (
            param.name != "self"
            and param.name not in parsed_param_types
            and (callable_name == "__init__" or not callable_name.startswith("__"))
        ):
            raise ValueError(
                f"The parameter {param.name} of {callable_name} has no type definition in the function documentation"
            )
        param_ast = ast.arg(
            arg=param.name, annotation=parsed_param_types.get(param.name)
        )

        default_ast = None
        if param.default != param.empty:
            default_ast = ast.Constant(param.default)
            if param.name not in optional_params:
                raise ValueError(
                    f"Parameter {param.name} is optional according to the type but not flagged as such in the doc"
                )
        elif param.name in optional_params:
            raise ValueError(
                f"Parameter {param.name} is optional according to the documentation but has no default value"
            )

        if param.kind == param.POSITIONAL_ONLY:
            posonlyargs.append(param_ast)
            defaults.append(default_ast)
        elif param.kind == param.POSITIONAL_OR_KEYWORD:
            args.append(param_ast)
            defaults.append(default_ast)
        elif param.kind == param.VAR_POSITIONAL:
            vararg = param_ast
        elif param.kind == param.KEYWORD_ONLY:
            kwonlyargs.append(param_ast)
            kw_defaults.append(default_ast)
        elif param.kind == param.VAR_KEYWORD:
            kwarg = param_ast

    return ast.arguments(
        posonlyargs=posonlyargs,
        args=args,
        vararg=vararg,
        kwonlyargs=kwonlyargs,
        kw_defaults=kw_defaults,
        defaults=defaults,
        kwarg=kwarg,
    )


def returns_stub(doc: str, types_to_import: Set[str]):
    m = re.findall(r"\n *:rtype: *([^\n]*) *\n", doc)
    if len(m) == 0:
        return None
    elif len(m) == 1:
        return convert_type_from_doc(m[0], types_to_import)
    else:
        raise ValueError("Multiple return type annotations found with :rtype:")


def convert_type_from_doc(type_str: str, types_to_import: Set[str]):
    type_str = type_str.strip()
    return parse_type_to_ast(type_str, types_to_import)


def parse_type_to_ast(type_str: str, types_to_import: Set[str]):
    # let's tokenize
    tokens = []
    current_token = ""
    for c in type_str:
        if "a" <= c <= "z" or "A" <= c <= "Z" or c == ".":
            current_token += c
        else:
            if current_token:
                tokens.append(current_token)
            current_token = ""
            if c != " ":
                tokens.append(c)
    if current_token:
        tokens.append(current_token)

    # let's first parse nested parenthesis
    stack: List[List[Any]] = [[]]
    for token in tokens:
        if token == "(":
            l: List[str] = []
            stack[-1].append(l)
            stack.append(l)
        elif token == ")":
            stack.pop()
        else:
            stack[-1].append(token)

    # then it's easy
    def parse_sequence(sequence):
        # we split based on "or"
        or_groups = [[]]
        for e in sequence:
            if e == "or":
                or_groups.append([])
            else:
                or_groups[-1].append(e)
        if any(not g for g in or_groups):
            raise ValueError(f'Not able to parse type "{type_str}"')

        new_elements = []
        for group in or_groups:
            if len(group) == 1 and isinstance(group[0], str):
                parts = group[0].split(".")
                if any(not p for p in parts):
                    raise ValueError(f'Not able to parse type "{type_str}"')
                if len(parts) > 1:
                    types_to_import.add(parts[0])
                new_elements.append(
                    reduce(
                        lambda acc, n: ast.Attribute(value=acc, attr=n, ctx=AST_LOAD),
                        parts[1:],
                        ast.Name(id=parts[0], ctx=AST_LOAD),
                    )
                )
            elif (
                len(group) == 2
                and isinstance(group[0], str)
                and isinstance(group[1], list)
            ):
                if group[0] not in GENERICS:
                    raise ValueError(
                        f'Constructor {group[0]} is not supported in type "{type_str}"'
                    )
                new_elements.append(
                    ast.Subscript(
                        value=GENERICS[group[0]],
                        slice=parse_sequence(group[1]),
                        ctx=AST_LOAD,
                    )
                )
            else:
                raise ValueError(f'Not able to parse type "{type_str}"')
        return (
            ast.Subscript(
                value=ast.Attribute(
                    value=ast.Name(id="typing", ctx=AST_LOAD),
                    attr="Union",
                    ctx=AST_LOAD,
                ),
                slice=ast.Tuple(elts=new_elements, ctx=AST_LOAD),
                ctx=AST_LOAD,
            )
            if len(new_elements) > 1
            else new_elements[0]
        )

    return parse_sequence(stack[0])


def build_doc_comment(doc: str):
    lines = [l.strip() for l in doc.split("\n")]
    clean_lines = []
    for l in lines:
        if l.startswith(":type") or l.startswith(":rtype"):
            continue
        else:
            clean_lines.append(l)
    return ast.Expr(value=ast.Constant("\n".join(clean_lines).strip()))


def format_with_black(code: str) -> str:
    result = subprocess.run(
        ["python", "-m", "black", "-t", "py37", "--pyi", "-"],
        input=code.encode(),
        capture_output=True,
    )
    result.check_returncode()
    return result.stdout.decode()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Extract Python type stub from a python module."
    )
    parser.add_argument(
        "module_name", help="Name of the Python module for which generate stubs"
    )
    parser.add_argument(
        "out",
        help="Name of the Python stub file to write to",
        type=argparse.FileType("wt"),
    )
    parser.add_argument(
        "--black", help="Formats the generated stubs using Black", action="store_true"
    )
    args = parser.parse_args()
    stub_content = ast.unparse(module_stubs(importlib.import_module(args.module_name)))
    if args.black:
        stub_content = format_with_black(stub_content)
    args.out.write(stub_content)
