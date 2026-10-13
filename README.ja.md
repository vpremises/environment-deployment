# 環境配置の共通基盤

環境設定の型・スキーマ・検証・解決・差分計画・実行結果の台帳・保守時間帯の計算を `crates/environment-deployment` にまとめます。Incus固有の検証とOpenTofu JSON生成は `crates/incus-renderer` に配置し、共通の解決済み型を直接利用します。契約用の別packageや同じ型の部分的な複製は置きません。

```sh
cargo +1.97 test --locked --workspace --all-targets
cargo +1.97 run --locked -p vpremises-environment-deployment --example reconciliation
```

playgroundは合成データとメモリ内SQLiteを使い、計画・receipt照合・再送・変更・ロールバック計画を説明します。実アカウントへの接続、配置、保守timerの有効化は行いません。

公開範囲は所有者がApache-2.0での公開を承認した共通ソースと合成fixtureです。非公開Git履歴・実profile・登録・運用データを含めません。製品CLIは `vpremises/environment-operations`、実設定は非公開の `vpremises/environment-configurations` が所有します。crateは現時点では `publish = false` とし、公開Gitを配布経路にします。
