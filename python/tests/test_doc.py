import inspect
from doctest import DocTest, DocTestFinder, DocTestSuite
from typing import Any, Dict, List, Optional
from unittest import TestLoader, TestSuite

import pyoxigraph


class ExtendedDocTestFinder(DocTestFinder):
    """
    More aggressive doctest lookup
    """

    def _find(
        self,
        tests: List[DocTest],
        obj: Any,
        name: Any,
        module: Any,
        source_lines: Any,
        globs: Any,
        seen: Dict[int, Any],
    ) -> None:
        # If we've already processed this object, then ignore it.
        if id(obj) in seen:
            return
        seen[id(obj)] = 1

        # Find a test for this object, and add it to the list of tests.
        test = self._get_test(obj, name, module, globs, source_lines)  # type: ignore[attr-defined]
        if test is not None:
            tests.append(test)

        # Look for tests in a module's contained objects.
        if inspect.ismodule(obj) or inspect.isclass(obj):
            for valname, val in obj.__dict__.items():
                if valname == "__doc__":
                    continue
                # Special handling for staticmethod/classmethod.
                if isinstance(val, (staticmethod, classmethod)):
                    val = val.__func__
                self._find(tests, val, f"{name}.{valname}", module, source_lines, globs, seen)


def load_tests(_loader: TestLoader, tests: TestSuite, _pattern: Optional[str] = None) -> TestSuite:
    tests.addTests(DocTestSuite(pyoxigraph, test_finder=ExtendedDocTestFinder()))
    return tests
