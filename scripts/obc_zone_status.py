"""Editable CDL realization of pinned OBC G36 ZoneGroups.ZoneStatus.

Source: modelica-buildings commit d3d8e6f76514dfbc7d65574c6b5a777b686fe1b7,
Buildings/Controls/OBC/ASHRAE/G36/ZoneGroups/ZoneStatus.mo. The optional
window sensor is represented by a Boolean input; pass true for a zone without
a sensor. All thresholds and conversion values remain editable block parameters.
"""

from control_graph import block, composite


def zone_status():
    real_inputs = (
        "cooDowTim", "warUpTim", "TZon", "TOccHeaSet", "TOccCooSet",
        "TUnoHeaSet", "TUnoCooSet",
    )
    boolean_outputs = (
        "yOccHeaHig", "yHigOccCoo", "yUnoHeaHig", "yEndSetBac",
        "yHigUnoCoo", "yEndSetUp",
    )
    blocks = [
        block("winOpe", "Buildings.Controls.OBC.CDL.Logical.Not", ("u",), boolean_ins=("u",), boolean_outs=("y",)),
        block("not1", "Buildings.Controls.OBC.CDL.Logical.Not", ("u",), boolean_ins=("u",), boolean_outs=("y",)),
        block("booToRea", "Buildings.Controls.OBC.CDL.Conversions.BooleanToReal", ("u",), params={"realTrue": 1.0, "realFalse": 0.0}, boolean_ins=("u",)),
        block("pro", "Buildings.Controls.OBC.CDL.Reals.Multiply", ("u1", "u2")),
        block("pro1", "Buildings.Controls.OBC.CDL.Reals.Multiply", ("u1", "u2")),
    ]
    for name in ("sub", "sub1", "sub2", "sub3", "sub4", "sub5"):
        blocks.append(block(name, "Buildings.Controls.OBC.CDL.Reals.Subtract", ("u1", "u2")))
    for name in ("hys", "hys1", "hys2", "hys5"):
        blocks.append(block(name, "Buildings.Controls.OBC.CDL.Reals.Hysteresis", ("u",),
                            params={"uLow": -0.1, "uHigh": 0.1}, boolean_outs=("y",)))
    for name in ("hys3", "hys4"):
        blocks.append(block(name, "Buildings.Controls.OBC.CDL.Reals.Hysteresis", ("u",),
                            params={"uLow": 0.0, "uHigh": 1.0}, boolean_outs=("y",)))
    for name in ("and1", "and2", "and3", "and5"):
        blocks.append(block(name, "Buildings.Controls.OBC.CDL.Logical.And", ("u1", "u2"),
                            boolean_ins=("u1", "u2"), boolean_outs=("y",)))
    for name in ("or1", "or2"):
        blocks.append(block(name, "Buildings.Controls.OBC.CDL.Logical.Or", ("u1", "u2"),
                            boolean_ins=("u1", "u2"), boolean_outs=("y",)))
    edges = [
        ("u1Win", "winOpe.u"), ("winOpe.y", "not1.u"),
        ("not1.y", "booToRea.u"),
        ("cooDowTim", "pro.u1"), ("warUpTim", "pro1.u1"),
        ("booToRea.y", "pro.u2"), ("booToRea.y", "pro1.u2"),
        ("pro.y", "yCooTim"), ("pro1.y", "yWarTim"),
        ("TOccHeaSet", "sub.u1"), ("TZon", "sub.u2"),
        ("TZon", "sub1.u1"), ("TOccCooSet", "sub1.u2"),
        ("TUnoHeaSet", "sub2.u1"), ("TZon", "sub2.u2"),
        ("TZon", "sub3.u1"), ("TUnoHeaSet", "sub3.u2"),
        ("TUnoCooSet", "sub4.u1"), ("TZon", "sub4.u2"),
        ("TZon", "sub5.u1"), ("TUnoCooSet", "sub5.u2"),
    ]
    for difference, hysteresis in (
        ("sub", "hys"), ("sub1", "hys1"), ("sub2", "hys2"),
        ("sub3", "hys3"), ("sub4", "hys4"), ("sub5", "hys5"),
    ):
        edges.append((f"{difference}.y", f"{hysteresis}.u"))
    for hysteresis, gate, output in (
        ("hys", "and2", "yOccHeaHig"),
        ("hys1", "and1", "yHigOccCoo"),
        ("hys2", "and3", "yUnoHeaHig"),
        ("hys5", "and5", "yHigUnoCoo"),
    ):
        edges.extend(((f"{hysteresis}.y", f"{gate}.u1"),
                      ("not1.y", f"{gate}.u2"),
                      (f"{gate}.y", output)))
    for hysteresis, gate, output in (
        ("hys3", "or2", "yEndSetBac"),
        ("hys4", "or1", "yEndSetUp"),
    ):
        edges.extend(((f"{hysteresis}.y", f"{gate}.u1"),
                      ("winOpe.y", f"{gate}.u2"),
                      (f"{gate}.y", output)))
    graph = composite(
        "OBCZoneStatus", "ZoneStatus",
        (*real_inputs, "u1Win"), ("yCooTim", "yWarTim", *boolean_outputs),
        blocks, edges,
        "G36 zone temperature, recovery time, and window status from editable CDL blocks.",
    )
    graph["notes"] = "Implements the pinned OBC ZoneStatus wiring. Pass u1Win=true when there is no window sensor."
    for port in graph["inputs"]:
        if port["name"] == "u1Win":
            port["type"] = "Boolean"
        elif port["name"] in ("cooDowTim", "warUpTim"):
            port["unit"] = "s"
        else:
            port["unit"] = "K"
    for port in graph["outputs"]:
        if port["name"] in boolean_outputs:
            port["type"] = "Boolean"
        else:
            port["unit"] = "s"
    return graph
