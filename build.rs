use std::process::Command;

fn compile_shader(input: &str, output: &str) {
    let status = Command::new("glslangValidator")
        .args(&["-V", "-o", output, input])
        .spawn()
        .expect("Error launching SPIRV validator")
        .wait()
        .unwrap();

    assert!(status.success());
}

fn main() {
    println!("cargo:rerun-if-changed=src/shader.vert");
    println!("cargo:rerun-if-changed=src/shader.frag");

    // comment these lines if you don't have `glslangValidator` in your PATH
    // (you won't be able to modify the shaders though)
    compile_shader("src/shader.vert", "src/shader.vert.spv");
    compile_shader("src/shader.frag", "src/shader.frag.spv");
}
