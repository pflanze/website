
- markdownprocessor etc. return Result, only at the very end we handle
  the errors and generate an error page. This is because why do it in
  multiple places, and 404 also needs to be handled at the outer side,
  so keep it consistent and do all errors there.
  

