_build cargo_opt:
    pushd ./minos
    cargo build {{ cargo_opt }}
    popd
