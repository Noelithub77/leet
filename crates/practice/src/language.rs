//! Shared language preferences for solutions, judging, and language servers.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    #[default]
    Python,
    Cpp,
    Go,
    C,
    Java,
}

impl Language {
    pub const ALL: [Self; 5] = [Self::Python, Self::Cpp, Self::Go, Self::C, Self::Java];
    pub fn label(self) -> &'static str { match self { Self::Python => "Python", Self::Cpp => "C++", Self::Go => "Go", Self::C => "C", Self::Java => "Java" } }
    pub fn id(self) -> &'static str { match self { Self::Python => "python", Self::Cpp => "cpp", Self::Go => "go", Self::C => "c", Self::Java => "java" } }
    pub fn extension(self) -> &'static str { match self { Self::Python => "py", Self::Cpp => "cpp", Self::Go => "go", Self::C => "c", Self::Java => "java" } }
    pub fn judge_id(self) -> &'static str { match self { Self::Python => "python3", Self::Cpp => "cpp", Self::Go => "golang", Self::C => "c", Self::Java => "java" } }
    pub fn next(self) -> Self { match self { Self::Python => Self::Cpp, Self::Cpp => Self::Go, Self::Go => Self::C, Self::C => Self::Java, Self::Java => Self::Python } }
    pub fn stdin_template(self) -> &'static str {
        match self {
            Self::Python => "import sys\n\ndef solve():\n    data = sys.stdin.read().split()\n    \n\nif __name__ == \"__main__\":\n    solve()\n",
            Self::Cpp => "#include <bits/stdc++.h>\nusing namespace std;\n\nint main() {\n    ios::sync_with_stdio(false);\n    cin.tie(nullptr);\n    \n    return 0;\n}\n",
            Self::Go => "package main\n\nimport (\n    \"bufio\"\n    \"os\"\n)\n\nfunc main() {\n    in := bufio.NewReader(os.Stdin)\n    out := bufio.NewWriter(os.Stdout)\n    defer out.Flush()\n    _ = in\n}\n",
            Self::Java => "import java.io.*;\nimport java.util.*;\n\npublic class Main {\n    public static void main(String[] args) throws Exception {\n        Scanner in = new Scanner(System.in);\n        \n    }\n}\n",
            Self::C => "#include <stdio.h>\n#include <stdlib.h>\n\nint main(void) {\n    \n    return 0;\n}\n",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    #[default]
    NeetCode,
    LeetCode,
    Codeforces,
}
impl Source {
    pub const ALL: [Self; 3] = [Self::NeetCode, Self::LeetCode, Self::Codeforces];
    pub fn label(self) -> &'static str { match self { Self::NeetCode => "NeetCode", Self::LeetCode => "LeetCode", Self::Codeforces => "Codeforces" } }
    pub fn next(self) -> Self { match self { Self::NeetCode => Self::LeetCode, Self::LeetCode => Self::Codeforces, Self::Codeforces => Self::NeetCode } }
}
