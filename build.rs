/*fn main() {
	cxx_build::bridge("structures/voxeldag.rs").std("c++20")
	.flag("-Wno-nonportable-include-path").flag("-Wno-ignored-qualifiers").flag("-Wno-unused-variable")
	.file("structures/voxeldag/storage.cpp")
	.compile("voxeldag");
}*/

fn main() -> miette::Result<()> {
	let path = std::path::PathBuf::from("structures"); // include path
	let mut b = autocxx_build::Builder::new("structures/voxeldag.rs", &[&path]).extra_clang_args(&["-std=c++20"]).build()?;
	b.flag("-std=c++20")
	.flag("-Wno-nonportable-include-path").flag("-Wno-ignored-qualifiers").flag("-Wno-unused-variable").flag("-Wno-delete-abstract-non-virtual-dtor").flag("-Wno-format-security")
	 .compile("voxeldag");
	Ok(())
}