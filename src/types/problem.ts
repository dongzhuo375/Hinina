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
}

export interface Sample {
  input: string
  output: string
}
