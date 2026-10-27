# The `eye/read` entry

Read a document and answer its text: a scan, a page image, a table, or a PDF read page by page.

```
bmo use eye/read --arg path=/home/…/scan.png
```

- **The answer is the text itself,** top to bottom, with headings and list marks, and a table as markdown.
  Left out, the question transcribes the page as it stands; a question asks something of it instead.
- **A PDF is read page by page.** `pages` narrows it (`1,3,5`), and the answer's `pages` carries one entry
  per page read.
- **What it is good at:** clean Korean documents, forms and tables. A folded, handwritten or photographed
  page is read worse: 300 tokens is the whole picture's budget on the node's reader.
