# type: ignore
import inspect
from doctest import DocTestFinder, DocTestSuite

import pyoxigraph


class ExtendedDocTestFinder(DocTestFinder):
    """
    More aggressive doctest lookup
    """

    def _find(self, tests, obj, name, module, source_lines, globs, seen):
        # If we've already processed this object, then ignore it.
        if id(obj) in seen:
            return
        seen[id(obj)] = 1

        # Find a test for this object, and add it to the list of tests.
        test = self._get_test(obj, name, module, globs, source_lines)
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


def load_tests(_loader, tests, _ignore):
    tests.addTests(DocTestSuite(pyoxigraph, test_finder=ExtendedDocTestFinder()))
    return tests
