import sys
from datetime import datetime, timezone
from pathlib import Path

import pyincroxigraph

sys.path.insert(0, str(Path(__file__).parent.parent.absolute()))

# -- Project information -----------------------------------------------------

project = "pyincroxigraph"
copyright = f"{datetime.now(tz=timezone.utc).year}, Incroxigraph and Oxigraph contributors"
version = pyincroxigraph.__version__
release = pyincroxigraph.__version__

# -- General configuration ---------------------------------------------------

extensions = ["sphinx.ext.autodoc", "sphinx.ext.doctest", "sphinx.ext.intersphinx"]

exclude_patterns = ["build", "Thumbs.db", ".DS_Store"]

# -- Options for HTML output -------------------------------------------------

html_theme = "furo"
html_static_path = []
html_logo = "../../logo.svg"
html_favicon = "../../logo.svg"
html_theme_options = {"body_max_width": None}

# -- Options for doctests -------------------------------------------------

doctest_global_setup = "from pyincroxigraph import *\nimport io"

# -- Options for intersphinx -------------------------------------------------

intersphinx_mapping = {"python": ("https://docs.python.org/3", None)}
