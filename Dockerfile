FROM rust:latest as builder
COPY . .
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    openssl \
    libssl-dev \
    pkg-config \
    ffmpeg

RUN curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp
RUN chmod a+rx /usr/local/bin/yt-dlp
RUN yt-dlp -U

RUN cargo update
RUN cargo build --release && mv ./target/release/jaseppi-v5 ./jaseppi-v5

# Run the app
CMD ./jaseppi-v5