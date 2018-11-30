from setuptools import setup, find_packages

setup(
    name="sonar",
    packages=find_packages(),
    entry_points={'console_scripts': ['sonar-snap = sonar.snap:take_snapshot']},
    install_requires=[
        'click==7.0',
    ],
)
