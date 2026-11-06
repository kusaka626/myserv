FROM myserv-native-check
RUN apt-get update && apt-get install -y --no-install-recommends gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64 && rm -rf /var/lib/apt/lists/*
