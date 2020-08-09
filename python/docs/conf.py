import datetime
import sys
from pathlib import Path

import pyoxigraph

sys.path.insert(0, str(Path(__file__).parent.parent.absolute()))

# -- Project information -----------------------------------------------------

project = "pyoxigraph"
copyright = f"{datetime.date.today().year}, Oxigraph contributors"
author = pyoxigraph.__author__
version = pyoxigraph.__version__
release = pyoxigraph.__version__

# -- General configuration ---------------------------------------------------

extensions = ["sphinx.ext.autodoc", "sphinx.ext.doctest", "sphinx.ext.intersphinx"]

exclude_patterns = ["build", "Thumbs.db", ".DS_Store"]

# -- Options for HTML output -------------------------------------------------

html_theme = "classic"
html_static_path = []
html_logo = "../../logo.svg"
html_favicon = "../../logo.svg"
html_theme_options = {"body_max_width": None}
html_baseurl = "https://oxigraph.org/pyoxigraph/stable/"

# -- Options for doctests -------------------------------------------------

doctest_global_setup = "from pyoxigraph import *\nimport io"

# -- Options for intersphinx -------------------------------------------------

intersphinx_mapping = {"python": ("https://docs.python.org/3", None)}
