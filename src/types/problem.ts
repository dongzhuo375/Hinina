/// 题目详情，对应 Rust `core::entity::problem::Problem`。
export interface Problem {
  id: string
  title: string
  description: string
  inputDescription: string
  outputDescription: string
  samples: Sample[]
  timeLimit: number
  memoryLimit: number
  /// 题目允许的提交语言（HOJ 显示名，如 "C++"），来自 get-contest-problem-details；
  /// 空数组 = 服务端未提供，消费方回退 utils/language 的 DEFAULT_LANGUAGES
  languages: string[]
}

export interface Sample {
  input: string
  output: string
}
