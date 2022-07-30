import inspect
from types import ModuleType
from typing import Any, List, Tuple

from stubdoc import stubdoc
from stubdoc import add_docstring_to_stubfile

with open("/workspaces/oxigraph/python/base.pyi", "r", encoding="utf-8") as file:
    python_stub_content = file.read()

    with open(
        "/workspaces/oxigraph/python/pyoxigraph.pyi", "w", encoding="utf-8"
    ) as file:
        file.write(
            "# This file was generated from python/base.pyi using generate_docs.py. \n"
            "# Do not modify this file directly.\n" + python_stub_content
        )


def _get_callable_names_from_module(module: ModuleType) -> List[str]:
    """
    Get callable names defined in specified module.

    Parameters
    ----------
    module : ModuleType
        Target module.

    Returns
    -------
    callable_names : list of str
        Result callable names in module str.
        If class method exists, name will be concatenated by comma.
        e.g., `_read_txt`, `SampleClass._read_text`.
        Nested function will not be included.
    """
    callable_names: List[str] = []
    members: List[Tuple[str, Any]] = inspect.getmembers(module)
    for member_name, member_val in members:
        if not hasattr(member_val, "__module__"):
            continue
        # if member_val.__module__ != module.__name__:
        #     continue
        if inspect.isfunction(member_val):
            callable_names.append(member_name)
            continue
        if inspect.isclass(member_val):
            from stubdoc.stubdoc import _append_class_callable_names_to_list

            _append_class_callable_names_to_list(
                callable_names=callable_names,
                class_name=member_name,
                class_val=member_val,
            )
            continue
    return callable_names


# monkey patch
stubdoc._get_callable_names_from_module = _get_callable_names_from_module

add_docstring_to_stubfile("pyoxigraph/pyoxigraph.py", "pyoxigraph.pyi")
