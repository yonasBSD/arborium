# Buck2 build file example
load("@prelude//rust:cargo_package.bzl", "cargo")
load("@prelude//rust:rust_binary.bzl", "rust_binary")
load("@prelude//rust:rust_library.bzl", "rust_library")

rust_library(
    name = "my_lib",
    srcs = glob(["src/**/*.rs"]),
    deps = [
        "//third-party:serde",
        "//third-party:tokio",
    ],
    features = ["async", "derive"],
    edition = "2021",
    visibility = ["PUBLIC"],
)

rust_binary(
    name = "my_app",
    srcs = ["src/main.rs"],
    deps = [":my_lib"],
    env = {
        "RUST_LOG": "info",
    },
)

# Platform-specific configuration
platform(
    name = "linux-x86_64",
    constraint_values = [
        "config//os:linux",
        "config//cpu:x86_64",
    ],
)

# Custom rule
def _my_rule_impl(ctx):
    out = ctx.actions.declare_output("output.txt")
    ctx.actions.run(
        ["process", ctx.attrs.src],
        env = {"OUT": out.as_output()},
    )
    return [DefaultInfo(default_output = out)]

my_rule = rule(
    impl = _my_rule_impl,
    attrs = {
        "src": attrs.source(),
    },
)
