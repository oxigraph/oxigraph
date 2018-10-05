from setuptools import setup

try:
    from setuptools_rust import Binding, RustExtension
except ImportError:
    print('You should install the setuptool-rust package to be able to build rudf')


setup(
    name="rudf",
    version="0.1",
    rust_extensions=[RustExtension("rudf.rudf", binding=Binding.RustCPython)],
    packages=["rudf"],
    zip_safe=False,
)