#!/usr/bin/env python3
"""Compare public Rust declarations with an independent, explicitly reviewed baseline."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parent.parent
BASELINE = ROOT / "contracts/rust-api.json"
# Keep strings intact while removing comments. Public signatures are recorded as
# tokens, so formatting and prose changes do not alter the compatibility surface.
LEXER = re.compile(r'''//[^\n]*|/\*.*?\*/|r\#*".*?"\#*|b?"(?:\\.|[^"\\])*"|b?'(?:\\.|[^'\\])'|[A-Za-z_]\w*|[0-9]+(?:\.[0-9]+)*|->|::|=>|[^\s]''', re.S)
PAIRS = {"(": ")", "[": "]", "{": "}"}


def tokens(text):
    return [m.group() for m in LEXER.finditer(text) if not m.group().startswith(("//", "/*"))]


def grouped_end(items, start):
    closing = PAIRS[items[start]]
    index = start + 1
    while index < len(items):
        if items[index] in PAIRS:
            index = grouped_end(items, index)
        elif items[index] == closing:
            return index + 1
        else:
            index += 1
    raise ValueError("An unbalanced Rust declaration cannot be inspected.")


def head_end(items, start, stops):
    index = start
    while index < len(items):
        if items[index] in stops:
            return index
        if items[index] in ("(", "["):
            index = grouped_end(items, index)
        else:
            index += 1
    return index


def attributes(items, start):
    result = []
    index = start
    while index + 1 < len(items) and items[index] == "#":
        offset = index + 1 + int(items[index + 1] == "!")
        if offset >= len(items) or items[offset] != "[":
            raise ValueError("An unsupported attribute cannot be inspected.")
        end = grouped_end(items, offset)
        if items[offset + 1] != "doc":
            result += items[index:end]
        index = end
    return result, index


def struct_fields(body):
    fields = []
    index = 0
    while index < len(body):
        attrs, index = attributes(body, index)
        start = index
        angle = 0
        while index < len(body):
            value = body[index]
            if value == "<":
                angle += 1
            elif value == ">":
                angle -= 1
            elif value == "," and angle == 0:
                break
            if value in PAIRS:
                index = grouped_end(body, index)
            else:
                index += 1
        field = body[start:index]
        if field[:1] == ["pub"] and field[1:2] != ["("]:
            fields.append(" ".join(attrs + field))
        index += 1
    return fields


def declarations(items, location, records, trait_methods=False):
    index = 0
    while index < len(items):
        attrs, index = attributes(items, index)
        if index >= len(items):
            break
        start = index
        public = items[index] == "pub" and items[index + 1:index + 2] != ["("]
        cursor = index + int(public)
        if items[index:index + 2] == ["pub", "("]:
            cursor = grouped_end(items, index + 1)
        kind_index = cursor
        while kind_index < len(items) and items[kind_index] in ("async", "const", "unsafe", "extern"):
            if items[kind_index] == "const" and items[kind_index + 1:kind_index + 2] != ["fn"]:
                break
            kind_index += 1
        kind = items[kind_index] if kind_index < len(items) else ""
        stops = {";"} if kind in ("use", "type", "const", "static") else {"{", ";"}
        end = head_end(items, start, stops)
        if end >= len(items):
            # Tuple structs and individual public fields are handled separately.
            if public:
                raise ValueError("An unsupported public declaration cannot be inspected in " + location)
            break
        header = items[start:end]
        has_body = items[end] == "{"
        after = grouped_end(items, end) if has_body else end + 1
        body = items[end + 1:after - 1] if has_body else []
        if kind == "impl":
            scope = " ".join(header)
            records.append({"scope": location, "kind": "implementation", "declaration": scope})
            declarations(body, location + " :: " + scope, records, "for" in header)
        elif public or (trait_methods and kind == "fn"):
            record = {"scope": location, "kind": kind, "declaration": " ".join(attrs + header)}
            if kind == "struct" and has_body:
                record["fields"] = struct_fields(body)
            elif kind == "enum":
                record["variants"] = " ".join(body)
            elif kind == "trait":
                record["members"] = []
                declarations(body, location + " :: " + " ".join(header), record["members"], True)
            elif kind == "mod" and has_body:
                declarations(body, location + " :: " + " ".join(header), records)
            records.append(record)
        index = after


def snapshot():
    records = []
    for path in sorted((ROOT / "src").glob("*.rs")):
        # The schema interpreter is crate-private and is not a Rust consumer API.
        if path.name == "contract.rs":
            continue
        declarations(tokens(path.read_text(encoding="utf-8")), "src/" + path.name, records)
    if not records:
        raise ValueError("No public Rust surface was found.")
    records.sort(key=lambda item: json.dumps(item, sort_keys=True))
    package = {}
    section = ""
    for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line.startswith("["):
            section = line
        match = re.fullmatch(r'(name|edition|rust-version)\s*=\s*("[^"]*")', line)
        if section == "[package]" and match:
            package[match.group(1)] = json.loads(match.group(2))
    if set(package) != {"name", "edition", "rust-version"}:
        raise ValueError("The crate identity, edition, and supported Rust version must be explicit.")
    return {"format": 2, "package": package, "declarations": records,
            "compiler_guarantees": compiler_guarantees(records)}


def compiler_guarantees(records):
    """Record public auto-trait obligations independently of private fields."""
    modules = {"client.rs": "", "error.rs": "", "optional.rs": "",
               "pagination.rs": "", "params_generated.rs": "params::",
               "models_generated.rs": "models::", "services_generated.rs": "services::"}
    types = set()
    for record in records:
        if record["kind"] not in {"struct", "enum", "type"} or " :: " in record["scope"]:
            continue
        module = modules.get(Path(record["scope"]).name)
        if module is None:
            raise ValueError("A public type needs an explicit compiler compatibility module.")
        match = re.search(r'\bpub\s+(?:struct|enum|type)\s+(\w+)', record["declaration"])
        if not match:
            raise ValueError("A public type cannot be represented by the compiler compatibility check.")
        name = match.group(1)
        if name in {"Optional", "ApiResponse", "Pager"}:
            name += "<String>"
        elif Path(record["scope"]).name == "services_generated.rs":
            name += "<'static>"
        elif "<" in record["declaration"][match.end():]:
            raise ValueError("A public generic type needs explicit compiler compatibility support.")
        types.add("raims::" + module + name)
    if "raims::RaimsClient" not in types:
        raise ValueError("The public client must retain its concurrency guarantees.")
    contract = json.loads((ROOT / "src/contract_generated.json").read_text(encoding="utf-8"))
    futures = []
    for service, methods in sorted(contract["services"].items()):
        accessor = ("simulations().charges()" if service == "simulation_charges" else
                    "simulations().refunds()" if service == "simulation_refunds" else service + "()")
        for method, operation_id in sorted(methods.items()):
            operation = contract["operations"][operation_id]
            args = ['"compatibility"' for _ in operation["pathParams"]]
            if operation["body"] or operation["query"]:
                args.append("&raims::params::" + operation_id + "Params::default()")
            for suffix in ("", "_with_options"):
                arguments = args + (["&raims::RequestOptions::default()"] if suffix else [])
                futures.append("client." + accessor + ".r#" + method + suffix + "(" + ", ".join(arguments) + ")")
    return {"send_sync_types": sorted(types), "send_futures": sorted(futures),
            "send_pagination_futures": ["pager.next_page()", "pager.previous_page()"]}


def check_compiler_guarantees(guarantees, output):
    """Materialize the compiler consumer exclusively outside the SDK checkout."""
    if not guarantees.get("send_sync_types") or not guarantees.get("send_futures"):
        raise ValueError("Compiler compatibility guarantees are missing from the independent baseline.")
    output = output.resolve()
    try:
        output.relative_to((ROOT.parent / ".tmp/raims-rust-sdk").resolve())
    except ValueError:
        raise ValueError("Compiler compatibility output must stay under the workspace SDK temporary directory.")
    output.mkdir(parents=True, exist_ok=False)
    (output / "src").mkdir()
    manifest = ('[package]\nname="raims-rust-api-compatibility-' + uuid.uuid4().hex + '"\nversion="0.0.0"\nedition="2021"\npublish=false\n'
                '[dependencies]\nraims={path=' + json.dumps(ROOT.as_posix()) + '}\n[workspace]\n')
    (output / "Cargo.toml").write_bytes(manifest.encode())
    (output / "Cargo.lock").write_bytes((ROOT / "Cargo.lock").read_bytes())
    lines = ["#![allow(dead_code)]", "fn send_sync<T: Send + Sync>() {}",
             "fn send_future<T: std::future::Future + Send>(_: T) {}", "fn main() {"]
    lines.extend("    send_sync::<" + name + ">();" for name in guarantees["send_sync_types"])
    lines.extend(["}", "fn futures(client: &raims::RaimsClient) {"])
    lines.extend("    send_future(" + expression + ");" for expression in guarantees["send_futures"])
    lines.extend(["}", "fn pagination_futures(pager: &mut raims::Pager<String>) {"])
    lines.extend("    send_future(" + expression + ");" for expression in guarantees["send_pagination_futures"])
    lines.append("}")
    (output / "src/main.rs").write_bytes(("\n".join(lines) + "\n").encode())
    env = {key: value for key, value in os.environ.items() if key.upper() not in {
        "RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}}
    env.update(PYTHONDONTWRITEBYTECODE="1")
    env.setdefault("CARGO_TARGET_DIR", str((ROOT.parent / ".tmp/raims-rust-sdk/target").resolve()))
    flags = {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == "nt" else {"start_new_session": True}
    log = output / "compiler.log"
    with log.open("wb") as stream:
        process = subprocess.Popen(["cargo", "check", "--offline", "--manifest-path", str(output / "Cargo.toml")],
                                   env=env, cwd=str(ROOT), stdin=subprocess.DEVNULL,
                                   stdout=stream, stderr=subprocess.STDOUT, **flags)
        try:
            code = process.wait(timeout=600)
        except BaseException:
            if os.name == "nt":
                try:
                    subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], check=True,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
                except subprocess.CalledProcessError:
                    if process.poll() is None:
                        raise
            else:
                os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=30)
            raise
    if code:
        raise ValueError("Compiler compatibility guarantees failed; see " + str(log) + "\n" + log.read_text(encoding="utf-8", errors="replace")[-5000:])
    print("Checked Send/Sync for {} public types and Send for {} request and pagination futures.".format(
        len(guarantees["send_sync_types"]), len(guarantees["send_futures"]) + len(guarantees["send_pagination_futures"])))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--check", action="store_true", help="Compare without modifying the baseline.")
    group.add_argument("--write", action="store_true", help="Explicitly accept the reviewed public surface.")
    parser.add_argument("--output", type=Path, help="New compiler consumer directory under the workspace SDK temporary directory.")
    args = parser.parse_args()
    current = snapshot()
    if args.write:
        check_compiler_guarantees(current["compiler_guarantees"], args.output or ROOT.parent / ".tmp/raims-rust-sdk" / ("api-" + uuid.uuid4().hex))
        BASELINE.write_bytes((json.dumps(current, indent=2, ensure_ascii=False) + "\n").encode("utf-8"))
        print("Recorded {} public Rust declarations.".format(len(current["declarations"])))
        return 0
    if not BASELINE.is_file():
        raise ValueError("The independent Rust API baseline is missing.")
    expected = json.loads(BASELINE.read_text(encoding="utf-8"))
    if current != expected:
        old = {json.dumps(row, sort_keys=True) for row in expected.get("declarations", [])}
        new = {json.dumps(row, sort_keys=True) for row in current["declarations"]}
        for label, rows in (("Removed or changed", old - new), ("Added or changed", new - old)):
            for row in sorted(rows)[:8]:
                print(label + ": " + row[:500], file=sys.stderr)
        raise ValueError("The public Rust surface differs from its reviewed baseline. Review compatibility before an explicit --write.")
    check_compiler_guarantees(expected["compiler_guarantees"], args.output or ROOT.parent / ".tmp/raims-rust-sdk" / ("api-" + uuid.uuid4().hex))
    print("Checked {} public Rust declarations.".format(len(current["declarations"])))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, KeyError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
