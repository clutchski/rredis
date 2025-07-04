.PHONY=watch,watch-test

watch:
	watchexec -e rs -c -r -- cargo run

watch-test:
	watchexec -e rs -c -r -- cargo test
