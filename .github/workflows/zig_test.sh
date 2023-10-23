cd /workdir || exit
python3.10 -m venv venv
source venv/bin/activate
pip install auditwheel
auditwheel show target/wheels/*.whl
pip install --no-index --find-links=target/wheels/ pyoxigraph
rm -r target/wheels
cd python/tests || exit
python -m unittest
