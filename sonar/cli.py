import click

from sonar.ps import print_ps_info
from sonar.summary import generate_summary


@click.group()
def group():
    pass


group.add_command(print_ps_info, name="ps")
group.add_command(generate_summary, name="summary")
