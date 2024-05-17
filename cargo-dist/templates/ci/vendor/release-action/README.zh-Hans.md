# Release Action

此操作将创建一个 GitHub Release，并可选择将产出文件上传到其中。

<div align="center">
<strong>
<samp>

[English](README.md) · [简体中文](README.zh-Hans.md)

</samp>
</strong>
</div>

## Action 输入

| 输入名称                   | 描述                                                                                                                                                                       | 必选  | 默认值               |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | -------------------- |
| allowUpdates               | 一个可选标志，表示如果版本已经存在，我们是否应该更新它。默认值为 false。                                                                                                   | false | ""                   |
| artifactErrorsFailBuild    | 一个可选标志，表示读取或上传产出文件错误时是否应该使构建失败。                                                                                                             | false | ""                   |
| artifacts                  | 一组可选的路径，表示要上传到版本的产出文件。 这可能是单个路径或以逗号分隔的路径列表（或 globs）                                                                            | false | ""                   |
| artifactContentType        | 产出文件的内容类型。 默认为 raw                                                                                                                                            | false | ""                   |
| body                       | 发布的可选主体。                                                                                                                                                           | false | ""                   |
| bodyFile                   | 发布的可选正文文件。 这应该是文件的路径。                                                                                                                                  | false | ""                   |
| commit                     | 一个可选的提交 ref。 如果标签不存在，将用于创建标签。                                                                                                                      | false | ""                   |
| discussionCategory         | 当提供该选项时，将生成对指定类别的 discussion。类别必须存在，否则将导致 Action 失败。这在草案发布中没有使用                                                                | false | ""                   |
| draft                      | 可选择将此版本标记为草稿版本。 设置为 true 以启用。                                                                                                                        | false | ""                   |
| generateReleaseNotes       | 指示是否应自动生成发行说明。                                                                                                                                               | false | false                |
| name                       | 版本的可选名称。 如果省略，将使用标签。                                                                                                                                    | false | ""                   |
| omitBody                   | 指示是否应省略发布主体。                                                                                                                                                   | false | false                |
| omitBodyDuringUpdate       | 指示在更新期间是否应省略发布主体。 正文仍将应用于新创建的版本。 这将在更新期间保留现有正文。                                                                               | false | false                |
| omitDraftDuringUpdate      | 指示是否应在更新期间省略草稿标志。 草稿标志仍将应用于新创建的版本。 这将在更新期间保留现有的草稿状态。                                                                     | false | false                |
| omitName                   | 指示是否应省略版本名称。                                                                                                                                                   | false | false                |
| omitNameDuringUpdate       | 指示在更新期间是否应省略版本名称。 该名称仍将应用于新创建的版本。 这将在更新期间保留现有名称。                                                                             | false | false                |
| omitPrereleaseDuringUpdate | 指示在更新期间是否应省略预发布标志。 预发布标志仍将应用于新创建的版本。 这将在更新期间保留现有的预发布状态。                                                               | false | false                |
| owner                      | （可选）指定应在其中生成版本的存储库的所有者。 默认为当前存储库的所有者。                                                                                                  | false | "current repo owner" |
| prerelease                 | 可选择将此版本标记为预发布。 设置为 true 以启用。                                                                                                                          | false | ""                   |
| removeArtifacts            | 指示是否应删除现有的发布产出文件。                                                                                                                                         | false | false                |
| replacesArtifacts          | 指示是否应替换现有的发布产出文件。                                                                                                                                         | false | true                 |
| repo                       | （可选）指定应在其中生成版本的存储库。                                                                                                                                     | false | current repo         |
| tag                        | 发布的可选标签。 如果省略，将使用 git ref （如果它是标签）。                                                                                                               | false | ""                   |
| token                      | GitHub 令牌。 这将默认为 GitHub 应用程序令牌。 如果您想使用您的个人令牌（用于定位其他存储库等），这主要是有用的。 如果您使用的是个人访问令牌，它应该可以访问 `repo` 范围。 | false | github.token         |
| updateOnlyUnreleased       | 启用 allowUpdates 后，如果它正在更新的版本不是草稿或预发布，则该操作将失败。                                                                                               | false | false                |

## Action 输出

| 输出名称   | 描述                     |
| ---------- | ------------------------ |
| id         | 创建的版本的标识符。     |
| html_url   | 版本的 HTML URL。        |
| upload_url | 将资产上传到版本的 URL。 |

## 示例

此示例将在推送一个标签时创建一个 Release：

```yml
name: Releases

on:
  push:
    tags:
      - "*"

jobs:
  build:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v2
      - uses: ncipollo/release-action@v1
        with:
          artifacts: "release.tar.gz,foo/*.txt"
          bodyFile: "body.md"
          token: ${{ secrets.YOUR_GITHUB_TOKEN }}
```

## 注意

- 您必须通过 Action 输入或 git ref 提供一个标签（即推送/创建标签）。如果不提供标签，Action 将会失败。
- 如果您正在创建的版本的标签不存在，您应该同时设置标签和提交 Action 输入。 commit 可以指向提交 Hash 或分支名称（例如 - main）。
- 在上面的示例中，只需要指定操作的权限（即 contents: write）。 如果您将其他操作添加到同一工作流程，则应相应地扩展权限。
