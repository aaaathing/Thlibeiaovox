
fn main() {
	cxx_build::bridge("structures/voxeldag.rs").std("c++20")
	.flag("-Wno-nonportable-include-path").flag("-Wno-ignored-qualifiers").flag("-Wno-unused-variable")
	.compile("voxeldag");
}