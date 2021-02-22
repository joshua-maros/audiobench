#!/usr/bin/env sh

rm -rf target/ || true
mkdir target/

cp src/* target/
rm target/decompress.sh
cp ../../artifacts/bin/Audiobench_Linux_x64_Standalone.bin target/Audiobench.bin
cp ../../artifacts/bin/Audiobench_Linux_x64_VST3.so target/Audiobench.so
cp ../../artifacts/bin/libaudiobench_clib.so target/
cp -r ../../dependencies/julia target/
rm target/julia/*.txt
rm target/julia/*.md

cd target
tar czf payload.tar.gz ./*
cd ..

mkdir -p ../../artifacts/installer/
rm ../../artifacts/installer/Installer_Linux_x64.sh || true
cat src/decompress.sh >> ../../artifacts/installer/Installer_Linux_x64.sh
cat target/payload.tar.gz >> ../../artifacts/installer/Installer_Linux_x64.sh
chmod +x ../../artifacts/installer/Installer_Linux_x64.sh

rm -rf target/
