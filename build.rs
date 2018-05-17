extern crate peg;

fn main() {
    peg::cargo_build("src/rio/ntriples/ntriples_grammar.rustpeg");
    peg::cargo_build("src/rio/turtle/turtle_grammar.rustpeg");
}
