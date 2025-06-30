.PHONY=watch,watch-test

watch:
	cargo watch -c -x run

watch-test:
	cargo watch -x test
