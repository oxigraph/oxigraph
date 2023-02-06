import argparse
import ast
import importlib
import inspect
import logging
import re
import subprocess
from typing import Set, List, Mapping, Any, Tuple, Union, Optional, Dict


def _path_to_type(*elements: str) -> ast.AST:
    base: ast.AST = ast.Name(id=elements[0], ctx=AST_LOAD)
    for e in elements[1:]:
        base = ast.Attribute(value=base, attr=e, ctx=AST_LOAD)
    return base


AST_LOAD = ast.Load()
AST_ELLIPSIS = ast.Ellipsis()
AST_STORE = ast.Store()
AST_TYPING_ANY = _path_to_type("typing", "Any")
GENERICS = {
    "iterable": _path_to_type("typing", "Iterable"),
    "iterator": _path_to_type("typing", "Iterator"),
    "list": _path_to_type("typing", "List"),
    "io": _path_to_type("typing", "IO"),
}
OBJECT_MEMBERS = dict(inspect.getmembers(object))


BUILTINS: Dict[str, Union[None, Tuple[List[ast.AST], ast.AST]]] = {
    "__annotations__": None,
    "__bool__": ([], _path_to_type("bool")),
    "__bytes__": ([], _path_to_type("bytes")),
    "__class__": None,
    "__contains__": ([AST_TYPING_ANY], _path_to_type("bool")),
    "__del__": None,
    "__delattr__": ([_path_to_type("str")], _path_to_type("None")),
    "__delitem__": ([AST_TYPING_ANY], AST_TYPING_ANY),
    "__dict__": None,
    "__dir__": None,
    "__doc__": None,
    "__eq__": ([AST_TYPING_ANY], _path_to_type("bool")),
    "__format__": ([_path_to_type("str")], _path_to_type("str")),
    "__ge__": ([AST_TYPING_ANY], _path_to_type("bool")),
    "__getattribute__": ([_path_to_type("str")], AST_TYPING_ANY),
    "__getitem__": ([AST_TYPING_ANY], AST_TYPING_ANY),
    "__gt__": ([AST_TYPING_ANY], _path_to_type("bool")),
    "__hash__": ([], _path_to_type("int")),
    "__init__": ([], _path_to_type("None")),
    "__init_subclass__": None,
    "__iter__": ([], AST_TYPING_ANY),
    "__le__": ([AST_TYPING_ANY], _path_to_type("bool")),
    "__len__": ([], _path_to_type("int")),
    "__lt__": ([AST_TYPING_ANY], _path_to_type("bool")),
    "__module__": None,
    "__ne__": ([AST_TYPING_ANY], _path_to_type("bool")),
    "__new__": None,
    "__next__": ([], AST_TYPING_ANY),
    "__reduce__": None,
    "__reduce_ex__": None,
    "__repr__": ([], _path_to_type("str")),
    "__setattr__": ([_path_to_type("str"), AST_TYPING_ANY], _path_to_type("None")),
    "__setitem__": ([AST_TYPING_ANY, AST_TYPING_ANY], AST_TYPING_ANY),
    "__sizeof__": None,
    "__str__": ([], _path_to_type("str")),
    "__subclasshook__": None,
}


def module_stubs(module: Any) -> ast.Module:
    types_to_import = {"typing"}
    classes = []
    functions = []
    for member_name, member_value in inspect.getmembers(module):
        element_path = [module.__name__, member_name]
        if member_name.startswith("__"):
            pass
        elif inspect.isclass(member_value):
            classes.append(
                class_stubs(member_name, member_value, element_path, types_to_import)
            )
        elif inspect.isbuiltin(member_value):
            functions.append(
                function_stub(member_name, member_value, element_path, types_to_import)
            )
        else:
            logging.warning(f"Unsupported root construction {member_name}")
    return ast.Module(
        body=[ast.Import(names=[ast.alias(name=t)]) for t in sorted(types_to_import)]
        + classes
        + functions,
        type_ignores=[],
    )


def class_stubs(
    cls_name: str, cls_def: Any, element_path: List[str], types_to_import: Set[str]
) -> ast.ClassDef:
    attributes: List[ast.AST] = []
    methods: List[ast.AST] = []
    magic_methods: List[ast.AST] = []
    for member_name, member_value in inspect.getmembers(cls_def):
        current_element_path = element_path + [member_name]
        if member_name == "__init__":
            try:
                inspect.signature(cls_def)  # we check it actually exists
                methods = [
                    function_stub(
                        member_name, cls_def, current_element_path, types_to_import
                    )
                ] + methods
            except ValueError as e:
                if "no signature found" not in str(e):
                    raise ValueError(
                        f"Error while parsing signature of {cls_name}.__init__: {e}"
                    )
        elif (
            member_value == OBJECT_MEMBERS.get(member_name)
            or BUILTINS.get(member_name, ()) is None
        ):
            pass
        elif inspect.isdatadescriptor(member_value):
            attributes.extend(
                data_descriptor_stub(
                    member_name, member_value, current_element_path, types_to_import
                )
            )
        elif inspect.isroutine(member_value):
            (magic_methods if member_name.startswith("__") else methods).append(
                function_stub(
                    member_name, member_value, current_element_path, types_to_import
                )
            )
        else:
            logging.warning(
                f"Unsupported member {member_name} of class {'.'.join(element_path)}"
            )

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
        decorator_list=[_path_to_type("typing", "final")],
    )


def data_descriptor_stub(
    data_desc_name: str,
    data_desc_def: Any,
    element_path: List[str],
    types_to_import: Set[str],
) -> Union[Tuple[ast.AnnAssign, ast.Expr], Tuple[ast.AnnAssign]]:
    annotation = None
    doc_comment = None

    doc = inspect.getdoc(data_desc_def)
    if doc is not None:
        annotation = returns_stub(data_desc_name, doc, element_path, types_to_import)
        m = re.findall(r"^ *:return: *(.*) *$", doc, re.MULTILINE)
        if len(m) == 1:
            doc_comment = m[0]
        elif len(m) > 1:
            raise ValueError(
                f"Multiple return annotations found with :return: in {'.'.join(element_path)} documentation"
            )

    assign = ast.AnnAssign(
        target=ast.Name(id=data_desc_name, ctx=AST_STORE),
        annotation=annotation or AST_TYPING_ANY,
        simple=1,
    )
    return (assign, build_doc_comment(doc_comment)) if doc_comment else (assign,)


def function_stub(
    fn_name: str, fn_def: Any, element_path: List[str], types_to_import: Set[str]
) -> ast.FunctionDef:
    body: List[ast.AST] = []
    doc = inspect.getdoc(fn_def)
    if doc is not None:
        body.append(build_doc_comment(doc))

    return ast.FunctionDef(
        fn_name,
        arguments_stub(fn_name, fn_def, doc or "", element_path, types_to_import),
        body or [AST_ELLIPSIS],
        decorator_list=[],
        returns=returns_stub(fn_name, doc, element_path, types_to_import)
        if doc
        else None,
        lineno=0,
    )


def arguments_stub(
    callable_name: str,
    callable_def: Any,
    doc: str,
    element_path: List[str],
    types_to_import: Set[str],
) -> ast.arguments:
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

    # Types for magic functions types
    builtin = BUILTINS.get(callable_name)
    if isinstance(builtin, tuple):
        param_names = list(real_parameters.keys())
        if param_names and param_names[0] == "self":
            del param_names[0]
        for name, t in zip(param_names, builtin[0]):
            parsed_param_types[name] = t

    # Types from comment
    for match in re.findall(r"^ *:type *([a-z_]+): ([^\n]*) *$", doc, re.MULTILINE):
        if match[0] not in real_parameters:
            raise ValueError(
                f"The parameter {match[0]} of {'.'.join(element_path)} is defined in the documentation but not in the function signature"
            )
        type = match[1]
        if type.endswith(", optional"):
            optional_params.add(match[0])
            type = type[:-10]
        parsed_param_types[match[0]] = convert_type_from_doc(
            type, element_path, types_to_import
        )

    # we parse the parameters
    posonlyargs = []
    args = []
    vararg = None
    kwonlyargs = []
    kw_defaults = []
    kwarg = None
    defaults = []
    for param in real_parameters.values():
        if param.name != "self" and param.name not in parsed_param_types:
            raise ValueError(
                f"The parameter {param.name} of {'.'.join(element_path)} has no type definition in the function documentation"
            )
        param_ast = ast.arg(
            arg=param.name, annotation=parsed_param_types.get(param.name)
        )

        default_ast = None
        if param.default != param.empty:
            default_ast = ast.Constant(param.default)
            if param.name not in optional_params:
                raise ValueError(
                    f"Parameter {param.name} of {'.'.join(element_path)} is optional according to the type but not flagged as such in the doc"
                )
        elif param.name in optional_params:
            raise ValueError(
                f"Parameter {param.name} of {'.'.join(element_path)} is optional according to the documentation but has no default value"
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


def returns_stub(
    callable_name: str, doc: str, element_path: List[str], types_to_import: Set[str]
) -> Optional[ast.AST]:
    m = re.findall(r"^ *:rtype: *([^\n]*) *$", doc, re.MULTILINE)
    if len(m) == 0:
        builtin = BUILTINS.get(callable_name)
        if isinstance(builtin, tuple) and builtin[1] is not None:
            return builtin[1]
        raise ValueError(
            f"The return type of {'.'.join(element_path)} has no type definition using :rtype: in the function documentation"
        )
    elif len(m) == 1:
        return convert_type_from_doc(m[0], element_path, types_to_import)
    else:
        raise ValueError(
            f"Multiple return type annotations found with :rtype: for {'.'.join(element_path)}"
        )


def convert_type_from_doc(
    type_str: str, element_path: List[str], types_to_import: Set[str]
) -> ast.AST:
    type_str = type_str.strip()
    return parse_type_to_ast(type_str, element_path, types_to_import)


def parse_type_to_ast(
    type_str: str, element_path: List[str], types_to_import: Set[str]
) -> ast.AST:
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
    def parse_sequence(sequence: List[Any]) -> ast.AST:
        # we split based on "or"
        or_groups: List[List[str]] = [[]]
        for e in sequence:
            if e == "or":
                or_groups.append([])
            else:
                or_groups[-1].append(e)
        if any(not g for g in or_groups):
            raise ValueError(
                f"Not able to parse type '{type_str}' used by {'.'.join(element_path)}"
            )

        new_elements: List[ast.AST] = []
        for group in or_groups:
            if len(group) == 1 and isinstance(group[0], str):
                parts = group[0].split(".")
                if any(not p for p in parts):
                    raise ValueError(
                        f"Not able to parse type '{type_str}' used by {'.'.join(element_path)}"
                    )
                if len(parts) > 1:
                    types_to_import.add(parts[0])
                new_elements.append(_path_to_type(*parts))
            elif (
                len(group) == 2
                and isinstance(group[0], str)
                and isinstance(group[1], list)
            ):
                if group[0] not in GENERICS:
                    raise ValueError(
                        f"Constructor {group[0]} is not supported in type '{type_str}' used by {'.'.join(element_path)}"
                    )
                new_elements.append(
                    ast.Subscript(
                        value=GENERICS[group[0]],
                        slice=parse_sequence(group[1]),
                        ctx=AST_LOAD,
                    )
                )
            else:
                raise ValueError(
                    f"Not able to parse type '{type_str}' used by {'.'.join(element_path)}"
                )
        return (
            ast.Subscript(
                value=_path_to_type("typing", "Union"),
                slice=ast.Tuple(elts=new_elements, ctx=AST_LOAD),
                ctx=AST_LOAD,
            )
            if len(new_elements) > 1
            else new_elements[0]
        )

    return parse_sequence(stack[0])


def build_doc_comment(doc: str) -> ast.Expr:
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
    stub_content = stub_content.replace(
        ", /", ""
    )  # TODO: remove when targeting Python 3.8+
    if args.black:
        stub_content = format_with_black(stub_content)
    args.out.write(stub_content)
