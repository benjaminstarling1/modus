# Simple Beam Example

A simply-supported beam (1.2 m long, 7 nodes) vibrating in its **first bending mode** at **2 Hz**.

## Files

| File | Purpose |
|---|---|
| `beam_nodes.csv` | 7 nodes (N1–N7) evenly spaced from X=0 to X=1.2 m along the X axis |
| `beam_edges.csv` | 6 edges connecting adjacent nodes into a continuous beam |
| `beam_data.csv` | 601 time steps (0–3 s @ 200 Hz), one displacement channel per node |

## Loading steps

1. **Import nodes** — Nodes tab → "⬇ Import CSV" → select `beam_nodes.csv`
2. **Import edges** — Edges tab → "⬇ Import CSV" → select `beam_edges.csv`
3. **Import data** — Left panel → "⊕ Import CSV / Parquet…" → select `beam_data.csv`
4. **Assign channels** — In the Nodes tab, for each node Ni set:
   - **dY** → `beam_data.csv::Ni_dy`
5. **Set units** — In the import panel, expand `beam_data.csv`; set each channel to **Disp / mm**
6. **Animate** — Bottom panel → press ▶

## What to expect

- The beam oscillates in the Y direction with a sinusoidal first-mode shape
- Node N4 (midpoint) deflects the most (~5 mm); N1 and N7 (supports) stay fixed
- Try **Contour Color** vis mode to see the red/blue magnitude gradient animate
- Use **Auto** displacement scale for an exaggerated view; untick for true-scale (5 mm)
