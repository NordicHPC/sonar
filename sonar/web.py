import os
from flask import Flask, render_template


def _get_subdir(dirname):
    return os.path.join(os.path.dirname(__file__), dirname)


def main(args):
    app = Flask('Sonar',
                template_folder=_get_subdir('templates'),
                static_folder=_get_subdir('static'))

    @app.route('/')
    def index():
        return render_template('data_visualisation.html')

    app.debug = args['debug']
    app.run(host=args['host'], port=args['port'])
