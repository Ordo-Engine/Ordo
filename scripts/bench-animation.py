"""
Ordo Benchmark Animation — Manim Scene
Generates a cinematic animated chart showing QPS vs Concurrency for multiple rule engines.

Usage:
    python3 -m manim -qh scripts/bench-animation.py BenchmarkRace    # 1080p MP4
    python3 -m manim -ql scripts/bench-animation.py BenchmarkRace    # quick preview
    python3 -m manim -qh --format gif scripts/bench-animation.py BenchmarkRace  # GIF
"""

from manim import *
import numpy as np

# ─── Benchmark Data (from hey, 10s per test) ───────────────────────────
CONCURRENCIES = [1, 5, 10, 25, 50, 100, 150, 200, 300, 500]

DATA = {
    "Ordo":               [13675, 33670, 46529, 53578, 59952, 60823, 61172, 61307, 61096, 62748],
    "OPA":                [ 9478, 23547, 25768, 29616, 30930, 33747, 35713, 35629, 37029, 37082],
    "json-rules-engine":  [11075, 19533, 21224, 22422, 22401, 22131, 21716, 21249, 17314, 14342],
    "Grule":              [ 5199,  7282,  5983,  5721,  6445,  7523,  7960,  8371,  8679,  8680],
}

COLORS = {
    "Ordo":              "#FF6B35",
    "OPA":               "#4ECDC4",
    "json-rules-engine": "#45B7D1",
    "Grule":             "#96CEB4",
}

SHORT_NAMES = {
    "Ordo": "Ordo",
    "OPA": "OPA",
    "json-rules-engine": "json-rules",
    "Grule": "Grule",
}


def interp_qps(engine, t):
    """Interpolate QPS at fractional index t (0..N-1)."""
    xs = list(range(len(CONCURRENCIES)))
    return float(np.interp(t, xs, DATA[engine]))


def interp_conc(t):
    """Interpolate concurrency at fractional index t."""
    xs = list(range(len(CONCURRENCIES)))
    return float(np.interp(t, xs, CONCURRENCIES))


class BenchmarkRace(Scene):
    def construct(self):
        self.camera.background_color = "#1a1a2e"
        n_points = len(CONCURRENCIES)

        # ─── Title ──────────────────────────────────────────────
        title = Text("Rule Engine Performance", font_size=40, color=WHITE, weight=BOLD)
        subtitle = Text(
            "QPS vs Concurrency  ·  4-branch decision rule  ·  Apple M1 Pro",
            font_size=18, color=GREY_B,
        )
        subtitle.next_to(title, DOWN, buff=0.2)
        title_group = VGroup(title, subtitle).to_edge(UP, buff=0.4)

        self.play(Write(title), FadeIn(subtitle, shift=UP * 0.3), run_time=1.2)
        self.wait(0.3)

        # ─── Axes ───────────────────────────────────────────────
        ax = Axes(
            x_range=[0, 550, 100],
            y_range=[0, 70000, 10000],
            x_length=9.5,
            y_length=4.8,
            axis_config={"color": GREY_B, "include_numbers": False, "tick_size": 0.05},
            tips=False,
        ).shift(DOWN * 0.4 + LEFT * 0.3)

        # X labels
        x_label = Text("Concurrency", font_size=15, color=GREY_B)
        x_label.next_to(ax.x_axis, DOWN, buff=0.35)
        x_ticks = VGroup()
        for c in [1, 50, 100, 200, 300, 500]:
            lbl = Text(str(c), font_size=12, color=GREY_C)
            lbl.next_to(ax.c2p(c, 0), DOWN, buff=0.12)
            x_ticks.add(lbl)

        # Y labels
        y_label = Text("QPS (requests/sec)", font_size=15, color=GREY_B)
        y_label.rotate(PI / 2).next_to(ax.y_axis, LEFT, buff=0.45)
        y_ticks = VGroup()
        for q in range(0, 80000, 10000):
            lbl = Text(f"{q // 1000}K", font_size=12, color=GREY_C)
            lbl.next_to(ax.c2p(0, q), LEFT, buff=0.12)
            y_ticks.add(lbl)

        # Grid
        grid = VGroup(*[
            DashedLine(ax.c2p(0, q), ax.c2p(550, q),
                       dash_length=0.04, color=GREY_D, stroke_opacity=0.25)
            for q in range(10000, 70001, 10000)
        ])

        self.play(
            Create(ax), FadeIn(grid),
            FadeIn(x_label), FadeIn(y_label),
            FadeIn(x_ticks), FadeIn(y_ticks),
            run_time=0.8,
        )

        # ─── Tracker ───────────────────────────────────────────
        tracker = ValueTracker(0)

        # ─── Traces (curves that grow) ─────────────────────────
        traces = {}
        for engine_name, color_hex in COLORS.items():
            color = ManimColor(color_hex)
            trace = VMobject(stroke_color=color, stroke_width=3, stroke_opacity=0.85)

            def make_updater(en, col):
                def updater(mob):
                    t = tracker.get_value()
                    if t < 0.01:
                        mob.clear_points()
                        return
                    pts = []
                    # All completed data points
                    idx_end = min(int(t), n_points - 1)
                    for i in range(idx_end + 1):
                        pts.append(ax.c2p(CONCURRENCIES[i], DATA[en][i]))
                    # Interpolated tip
                    if t > idx_end and idx_end < n_points - 1:
                        pts.append(ax.c2p(interp_conc(t), interp_qps(en, t)))
                    if len(pts) >= 2:
                        mob.set_points_as_corners(pts)
                    elif len(pts) == 1:
                        mob.set_points_as_corners([pts[0], pts[0]])
                return updater

            trace.add_updater(make_updater(engine_name, color))
            traces[engine_name] = trace
            self.add(trace)

        # ─── Dots + Labels ─────────────────────────────────────
        for engine_name, color_hex in COLORS.items():
            color = ManimColor(color_hex)
            sn = SHORT_NAMES[engine_name]

            dot = always_redraw(lambda en=engine_name, col=color: Dot(
                ax.c2p(
                    interp_conc(min(tracker.get_value(), n_points - 1)),
                    interp_qps(en, min(tracker.get_value(), n_points - 1)),
                ),
                radius=0.07, color=col,
            ))

            # Label direction: json-rules goes LEFT to avoid overlap with OPA
            label = always_redraw(lambda en=engine_name, sn=sn, col=color: Text(
                f"{sn}: {int(interp_qps(en, min(tracker.get_value(), n_points - 1))):,}",
                font_size=13, color=col, weight=BOLD,
            ).next_to(
                ax.c2p(
                    interp_conc(min(tracker.get_value(), n_points - 1)),
                    interp_qps(en, min(tracker.get_value(), n_points - 1)),
                ),
                RIGHT if en != "json-rules-engine" else LEFT,
                buff=0.12,
            ))

            self.add(dot, label)

        # ─── Animate: race! ────────────────────────────────────
        # Phase 1: slow start (C=1 → C=10)
        self.play(tracker.animate.set_value(2), run_time=2.0, rate_func=smooth)

        # Phase 2: ramp up (C=10 → C=100)
        self.play(tracker.animate.set_value(5), run_time=2.5, rate_func=smooth)

        # Mark Ordo saturation at C≈100
        sat_pt = ax.c2p(100, 60823)
        sat_dot = Dot(sat_pt, radius=0.1, color=YELLOW, fill_opacity=0.7)
        sat_label = Text("~60K saturated", font_size=13, color=YELLOW, weight=BOLD)
        sat_label.next_to(sat_pt, UP, buff=0.15)
        self.play(FadeIn(sat_dot, scale=1.5), Write(sat_label), run_time=0.7)
        self.wait(0.6)

        # Phase 3: continue to end (C=100 → C=500)
        self.play(tracker.animate.set_value(n_points - 1), run_time=3.0, rate_func=smooth)
        self.wait(0.3)

        # ─── Final frame: gap annotations (no overlap) ─────────
        # Vertical dashed line at C=500
        final_line = DashedLine(
            ax.c2p(500, 0), ax.c2p(500, 65000),
            dash_length=0.06, color=WHITE, stroke_opacity=0.3,
        )
        self.play(Create(final_line), run_time=0.4)

        # Show gap as right-side labels connected by horizontal arrows
        # Position them in a right column, vertically spaced to avoid overlap
        ordo_qps = DATA["Ordo"][-1]
        competitors = [
            ("OPA", DATA["OPA"][-1], COLORS["OPA"]),
            ("json-rules", DATA["json-rules-engine"][-1], COLORS["json-rules-engine"]),
            ("Grule", DATA["Grule"][-1], COLORS["Grule"]),
        ]

        gap_group = VGroup()
        # Place gap labels at fixed right-side positions, evenly spaced
        right_x = ax.c2p(550, 0)[0] + 0.6
        label_positions = [
            ax.c2p(0, 52000)[1],  # top slot (for OPA gap)
            ax.c2p(0, 35000)[1],  # middle slot (for json-rules gap)
            ax.c2p(0, 18000)[1],  # bottom slot (for Grule gap)
        ]

        for i, (name, qps, col_hex) in enumerate(competitors):
            ratio = ordo_qps / qps
            col = ManimColor(col_hex)

            # Arrow from engine's final point to the label
            engine_pt = ax.c2p(500, qps)
            label_y = label_positions[i]

            # Gap label
            gap_text = Text(
                f"Ordo {ratio:.1f}x > {name}",
                font_size=14, color=YELLOW, weight=BOLD,
            )
            gap_text.move_to([right_x + 0.8, label_y, 0])

            # Small connector line from the engine dot area to the label
            connector = Line(
                [engine_pt[0] + 0.1, engine_pt[1], 0],
                [right_x + 0.05, label_y, 0],
                color=col, stroke_width=1.5, stroke_opacity=0.5,
            )

            gap_group.add(connector, gap_text)

        self.play(
            LaggedStart(*[FadeIn(obj) for obj in gap_group], lag_ratio=0.2),
            run_time=1.5,
        )

        # Bottom summary
        summary = Text(
            f"Ordo: {ordo_qps:,} QPS  ·  1.7x OPA  ·  4.4x json-rules  ·  7.2x Grule",
            font_size=15, color=WHITE, weight=BOLD,
        )
        summary.to_edge(DOWN, buff=0.25)
        self.play(FadeIn(summary, shift=UP * 0.2), run_time=0.8)
        self.wait(2.5)

        self.play(*[FadeOut(mob) for mob in self.mobjects], run_time=1.0)
