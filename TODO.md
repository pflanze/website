# To do

## Various

* Range (https://en.wikipedia.org/wiki/List_of_HTTP_header_fields, https://en.wikipedia.org/wiki/Byte_serving) (for video streaming especially)

* Smooth server restarts (handing over listening socket via fork/exec or unix domain socket)

* The HTML element database is missing SVG!

## Potential ahtml cleanup

- Attributes: currently accepting slices/arrays of either all
  Allocator based values, or of all (KString, KString) values, right?
  Move to allow mixed *there*? (+ Similar for bodies?)

