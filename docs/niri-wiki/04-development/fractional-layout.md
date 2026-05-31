# Fractional Layout (Physical Pixel Alignment)

> How niri handles fractional scaling without visual artifacts.

## Problem

- Integer physical coordinates ≠ integer logical coordinates at scale > 1
- Fractional scales make it worse
- Artifacts: alternating border width, 1px offsets, gaps around rounded corners, blurry content

## niri's Solution

1. **Round all sizes to integer physical pixels:** `(logical_size * scale).round() / scale`
   - Applies to: struts, gaps, border widths, working area location
   - Re-rounded when workspace moves to output with different scale
   - Stays fractional in logical space, but corresponds to integer physical pixels

2. **Don't round the view offset** (continuous scrolling needs sub-pixel precision)

3. **Round tile render positions** to physical pixels via `tiles_with_render_positions()`

4. **Custom shaders** (open/close/resize) also round to physical pixels

The result: every tile assumes rendering at integer physical coordinates, so shifting by border width (also integer-rounded) stays aligned. The entire layout stays aligned because gaps, struts, and working area are all rounded the same way.
