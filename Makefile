debug:
	cargo build

release:
	cargo build --release

debug_windows:
	cargo build --target x86_64-pc-windows-gnu

release_windows:
	cargo build --release --target x86_64-pc-windows-gnu

test:
	cargo test
