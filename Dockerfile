# İnce bir temelden başlıyoruz
FROM scratch

# Derlediğimiz Wasm dosyasını konteyner içine kopyalıyoruz
COPY target/wasm32-wasip1/debug/isobrowse_test.wasm /isobrowse.wasm

# Docker'a bu dosyanın bir Wasm dosyası olduğunu söylüyoruz
ENTRYPOINT ["/isobrowse.wasm"]
