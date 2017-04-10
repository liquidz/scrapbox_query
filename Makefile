debug_linux:
	cargo build

debug_windows:
	cargo build --target x86_64-pc-windows-gnu

releast_linux:
	cargo build --release

releast_windows:
	cargo build --release --target x86_64-pc-windows-gnu

test:
	cargo test
