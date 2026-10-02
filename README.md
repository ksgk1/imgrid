# ImGrid

A simple console tool, that combines a set of images into one.

# Usage:

```bash
./imgrid 1.png 2.png 3.png 4.png
```

This will turn into the following resulting image:

```text
+-------+-------+
|       |       |
| 1.png | 2.png |
|       |       |
+-------+-------+
|       |       |
| 3.png | 4.png |
|       |       |
+-------+-------+
```

The order is taken from the input. The resulting image is called `grid.png` unsued space is filled with transparency. The resulting grid is not scaled down, and inputs are not scaled either. Ideally all input images have the same size and dimensions.