import json
import timeglyph


def test_identify_ranks_unix_first():
    r = timeglyph.identify("1577836800")
    assert any(x["format_id"] == "unix" for x in r)
    assert r[0]["citation"]


def test_identify_new_formats():
    assert any(x["format_id"] == "unix_float" for x in timeglyph.identify("1712345678.001200"))
    assert any(x["format_id"] == "pdf_date" for x in timeglyph.identify("D:20260709123456+08'00'"))


def test_identify_json_matches():
    r = json.loads(timeglyph.identify_json("1577836800"))
    assert any(x["format_id"] == "unix" for x in r)


def test_empty_for_junk():
    assert timeglyph.identify("not a timestamp !!!") == []
