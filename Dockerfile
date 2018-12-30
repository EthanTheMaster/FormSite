FROM rust:1.31

WORKDIR /usr/src/FormSite
COPY . .

RUN cargo install

CMD ["FormSite"]
