"""vg local LeetCode runner: python3 harness.py <solution.py> < spec.json.

The spec carries LeetCode's metaData and test cases; one JSON result line per case is printed.
"""

import collections, bisect, functools, heapq, io, itertools, json, math, operator, random, re, string, sys, time, traceback
from collections import *
from contextlib import redirect_stdout
from functools import *
from heapq import *
from itertools import *
from math import *
from typing import *
from bisect import *

sys.setrecursionlimit(1_000_000)


class ListNode:
    def __init__(self, val=0, next=None):
        self.val = val
        self.next = next


class TreeNode:
    def __init__(self, val=0, left=None, right=None):
        self.val = val
        self.left = left
        self.right = right


def to_list_node(values):
    head = tail = None
    for v in values or []:
        node = ListNode(v)
        if tail:
            tail.next = node
        else:
            head = node
        tail = node
    return head


def from_list_node(node):
    out, seen = [], 0
    while node and seen < 100_000:
        out.append(node.val)
        node, seen = node.next, seen + 1
    return out


def to_tree(values):
    if not values or values[0] is None:
        return None
    root = TreeNode(values[0])
    queue, i = collections.deque([root]), 1
    while queue and i < len(values):
        node = queue.popleft()
        for side in ("left", "right"):
            if i < len(values) and values[i] is not None:
                child = TreeNode(values[i])
                setattr(node, side, child)
                queue.append(child)
            i += 1
    return root


def from_tree(root):
    out, queue = [], collections.deque([root])
    while queue:
        node = queue.popleft()
        if node is None:
            out.append(None)
            continue
        out.append(node.val)
        queue.append(node.left)
        queue.append(node.right)
    while out and out[-1] is None:
        out.pop()
    return out


def decode(raw, kind):
    value = json.loads(raw)
    base = kind.rstrip("[]")
    depth = (len(kind) - len(base)) // 2
    if base == "ListNode":
        return _map_depth(value, depth, to_list_node)
    if base == "TreeNode":
        return _map_depth(value, depth, to_tree)
    return value


def _map_depth(value, depth, fn):
    if depth == 0:
        return fn(value)
    return [_map_depth(v, depth - 1, fn) for v in value]


def encode(value):
    if isinstance(value, ListNode):
        return from_list_node(value)
    if isinstance(value, TreeNode):
        return from_tree(value)
    if isinstance(value, (list, tuple)):
        return [encode(v) for v in value]
    if isinstance(value, (set, frozenset)):
        return [encode(v) for v in value]
    return value


def dump(value):
    return json.dumps(encode(value), separators=(",", ":"), ensure_ascii=False)


def run_function(module, meta, lines):
    params = meta.get("params", [])
    args = [decode(line, p["type"]) for line, p in zip(lines, params)]
    solution = module["Solution"]()
    result = getattr(solution, meta["name"])(*args)
    index = (meta.get("output") or {}).get("paramindex")
    if index is not None:
        return args[index]
    if meta.get("return", {}).get("type") == "void":
        return args[0] if args else None
    return result


def run_design(module, meta, lines):
    calls, call_args = json.loads(lines[0]), json.loads(lines[1])
    cls = module[meta.get("classname") or calls[0]]
    instance, out = None, []
    for name, args in zip(calls, call_args):
        if instance is None:
            instance = cls(*args)
            out.append(None)
        else:
            out.append(getattr(instance, name)(*args))
    return out


def main():
    spec = json.load(sys.stdin)
    path = sys.argv[1]
    module = {"__name__": "solution", "__file__": path}
    module.update({k: v for k, v in globals().items() if not k.startswith("__")})
    try:
        with open(path, encoding="utf-8") as f:
            code = compile(f.read(), path, "exec")
        exec(code, module)
    except BaseException:
        print(json.dumps({"compile_error": traceback.format_exc(limit=-3)}), flush=True)
        return
    meta = spec["meta"]
    design = bool(meta.get("systemdesign")) or "classname" in meta
    if not design:
        expected = meta.get("name", "")
        solution_class = module.get("Solution")
        if not callable(solution_class) or not callable(getattr(solution_class, expected, None)):
            available = [name for name in vars(solution_class or {}).keys() if not name.startswith("_")] if isinstance(solution_class, type) else []
            message = f"Wrong solution interface: this question requires Solution.{expected}(...)."
            if available:
                message += " Found: " + ", ".join(available) + "."
            message += " Check that the code belongs to the active question, or request a new Solution only answer."
            print(json.dumps({"compile_error": message}), flush=True)
            return
    for case in spec["cases"]:
        stdout = io.StringIO()
        start = time.perf_counter()
        record = {"id": case["id"]}
        try:
            with redirect_stdout(stdout):
                value = (run_design if design else run_function)(module, meta, case["input"])
            record["output"] = dump(value)
        except BaseException:
            record["error"] = traceback.format_exc(limit=-4)
        record["ms"] = (time.perf_counter() - start) * 1000
        record["stdout"] = stdout.getvalue()[-4000:]
        print(json.dumps(record), flush=True)


main()
