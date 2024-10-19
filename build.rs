fn main() -> miette::Result<()> {
	let path = std::path::PathBuf::from("src/structures"); // include path
	let mut b = autocxx_build::Builder::new("src/structures/voxeldag.rs", &[&path])
	.extra_clang_args(&["-std=c++20","--sysroot=/Users/aaron/Downloads/wasi-sdk/share/wasi-sysroot"])
	.build()?;
	b.flag("-std=c++20")
	.flag("-Wno-nonportable-include-path").flag("-Wno-ignored-qualifiers").flag("-Wno-unused-variable").flag("-Wno-delete-abstract-non-virtual-dtor").flag("-Wno-format-security")
	 .compile("voxeldag");
	Ok(())
}