# Use an official Rust image as the base image
FROM rust:latest as builder

# Set the working directory inside the container
WORKDIR /app

# Copy the Rust toolchain files and source code into the container
COPY . .

# Build your application in release mode
RUN cargo build --release --bin default-pbs

# Use Ubuntu as the runtime base image
FROM ubuntu:latest as runtime

# Set the working directory to /app
WORKDIR /app

# Install necessary packages
RUN apt-get update && apt-get install -y \
  openssl \
  ca-certificates \
  libssl3 \
  libssl-dev \
  && rm -rf /var/lib/apt/lists/*

ENV CB_CONFIG="/app/config.toml"
ENV METRICS_SERVER="10000"

# Copy the binary from the builder stage to the runtime stage
COPY --from=builder /app/target/release/default-pbs /usr/local/bin

# Copy the configuration file into the container
COPY config.toml /app/config.toml

# Set the entrypoint for the container
ENTRYPOINT ["/usr/local/bin/default-pbs"]
