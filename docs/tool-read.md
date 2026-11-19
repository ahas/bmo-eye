# The `eye/read` entry

Read a document and answer its text: a scan, a page image, a table, or a PDF read page by page.

```
bmo use eye/read --arg path=/home/…/scan.png
```

- **The answer is the text itself,** top to bottom, with headings and list marks, and a table as markdown.
  Left out, the question transcribes the page as it stands; a question asks something of it instead.
- **A PDF is read page by page.** `pages` narrows it (`1,3,5`), and the answer's `pages` carries one entry
  per page read.
- **When no backend can answer:** the document is handed to you: the answer gives the question and the
  file's path, with a page image itself where it is small enough to send, and why every backend failed. Open
  the file yourself and read it. Called from a shell, the call answers an error with every backend's reason
  instead (`docs/api.md`, 「Handing the input to the caller」).
- **What it is good at:** clean Korean documents, forms and tables. A folded, handwritten or photographed
  page is read worse: 300 tokens is the whole picture's budget on the node's reader.
