# contest

`contest.current(context)` / `contest.get(context, contestId)` 需 contest.read。无当前比赛或请求其他比赛返回 null；旧 context 返回 context_expired。只读 Service 方法不调用 load_contest_with_problems，不会选择或切换比赛。

返回 contestId、纯文本 title（最多256字符）、startsAt、endsAt（UTC毫秒），不返回密码、HTML 描述或完整领域实体。
