default:
	@echo "Run 'make test' or 'make test_website' or just './deploy'"

test:
	RUST_BACKTRACE=1 cargo test --color=always
	RUST_BACKTRACE=1 cargo test --color=always -p chj-util
	RUST_BACKTRACE=1 cargo test --color=always -p ahtml_html
	RUST_BACKTRACE=1 cargo test --color=always -p ahtml

test_website:
	RUST_BACKTRACE=1 cargo test --color=always --lib 
	RUST_BACKTRACE=1 cargo test --color=always --bin website
