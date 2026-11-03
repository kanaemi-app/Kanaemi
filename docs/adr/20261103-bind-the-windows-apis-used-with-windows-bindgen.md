# Windows の API は、使うものだけを windows-bindgen で生成して呼ぶ

状態：採用

## 背景

Windows の入力メソッドは、Windows の API を `windows` クレートから呼んでいた。`windows` は、COM の実装に使う `windows-core` を自分の依存に持ち、その版に縛られる。2 つの版が食い違うと、COM のインターフェイスを実装できず、ビルドが通らない。

windows-rs は、API の定義を SDK のヘッダから読み直した新しい版を出した。`windows-core` などの小さなクレートは新しい版で公開されたが、API 全体を持つ `windows` は確かめきれていないとして公開されず、いつ出るかも示されていない。`windows` を使い続ける限り、`windows-core` は古い版から上げられない。windows-rs も、広い `windows` ではなく、要る API だけを `windows-bindgen` で生成することを勧めている。

## 決定

Windows の API は `windows` クレートから呼ばず、使うものだけを `windows-bindgen` でビルドのたびに生成して呼ぶ。

- 生成したものが依るのは `windows-core` だけで、その版は `windows` を待たずに上げる。
- 新しい定義に入っていない API は、SDK のヘッダから手で書き足す。

## 検討した他の案

- `windows` を使い続け、新しい版が公開されるのを待つ：いつ出るか分からず、その間 `windows-core` を上げられない。次の版でも同じ待ちが起こりうる。
- `windows-sys` のような、型を持たない生の宣言だけを生成する：COM のインターフェイスを実装する `windows-core` の仕組みが使えず、TSF のテキストサービスを手で組むことになる。
- 生成したものをリポジトリに置く：生成元の版を上げるたびに大きな差分を見直すことになる。ビルドのたびに生成しても、時間はほとんどかからない。

## 結果

- `windows-core` は、`windows-bindgen` と同じ版にそろえて上げる。生成されるものは、同じ版の `windows-core` に合わせて書かれる。
- 新しい定義が欠けている API は、定義が追いつくまで、手で書いた宣言として持つ。
