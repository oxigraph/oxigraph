fn main() {
    protobuf_codegen::Codegen::new()
        .pure()
        .cargo_out_dir("jelly-rdf")
        .input("proto/rdf.proto")
        .include("proto")
        .run_from_script();
}
