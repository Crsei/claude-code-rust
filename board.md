在同一个工作区里面应该记录刚才的权限计划
输入/开头的命令时不能只展示[args]。应该展示具体的选项或者示例用法，

输入/开头的命令出现的commands展示框会覆盖上方对话，修改为在下方展示，
然后现在在commands命令框开启时滑动只显示最初的几条命令

 /model, 将展示的内容中修改为用户提供的模型，不展示opus,sonnet,haiku,

Info: Current model: deepseek-v4-pro

    opus -> claude-opus-4-20250514
    sonnet -> claude-sonnet-4-20250514
    haiku -> claude-haiku-3-5-20241022

将模型的思考默认显示思考 多少秒，实现一个快捷键展示思考的内容

输入command 时 commands展示框不跳转到对应的command

commands 的顺序按照字母顺序从小到大，/init在最前面

/resume 一个被/clear的session时会报错，/clear修改为启动新会话,

/init 应该生成CLAUDE.md


启动项目时应该有：

────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
 Accessing workspace:

 F:\temp\gomoku_subagent

 Quick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source
 project, or work from your team). If not, take a moment to review what's in this folder first.

 Claude Code'll be able to read, edit, and execute files here.

 Security guide

 ❯ 1. Yes, I trust this folder
   2. No, exit

 Enter to confirm · Esc to cancel