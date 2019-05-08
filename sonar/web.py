import os
from flask import Flask, render_template, jsonify


def _generate_example_data():
    # FIXME: later we will read this from files
    data = {}
    data["Vasp"] = {
        "percent": 24.3,
        "subcalls": {
            "vasp_std": 43.6,
            "vasp.5.3.2": 16.8,
            "std": 15.6,
            "vasp.NGZhalf": 10.3,
            "vasp.5.3.5": 9.5,
        },
    }
    data["foo"] = {
        "percent": 24.3,
        "subcalls": {
            "vasp_std": 13.6,
            "vasp.5.3.2": 16.8,
            "std": 35.6,
            "vasp.NGZhalf": 10.3,
            "vasp.5.3.5": 9.5,
        },
    }
    return data


def _get_subdir(dirname):
    return os.path.join(os.path.dirname(__file__), dirname)


def main(args):
    app = Flask(
        "Sonar",
        template_folder=_get_subdir("templates"),
        static_folder=_get_subdir("static"),
    )

    @app.route("/")
    def index():
        return render_template("data_visualisation.html")

    @app.route("/data/example/", methods=["GET"])
    def get_example():
        return jsonify({"software": _generate_example_data()})

    app.debug = args["debug"]
    app.run(host=args["host"], port=args["port"])
