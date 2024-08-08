images:
	bash ./scripts/build_local_images.sh

module:
	bash ./scripts/build_local_modules.sh

docker: images module

start: docker
	cargo run --bin commit-boost -- init --config config.example.toml
	cargo run --bin commit-boost -- start --docker "./cb.docker-compose.yml" --env "./.cb.env"

stop:
	cargo run --bin commit-boost -- stop