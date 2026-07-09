_build cargo_opt:
    pushd ./minos
    cargo build {{ cargo_opt }}
    popd

# Build the release binary and package it into a Windows MSI with WiX.
# Requires the WiX toolset:  dotnet tool install --global wix
msi version="0.1.0": (_build "--release")
    wix build ./minos/wix/minos.wxs -b ./minos/target/release -d Version={{ version }} -o ./minos/target/wix/minos.msi
