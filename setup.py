from setuptools import setup, find_packages

setup(
    name="sonar",
    packages=find_packages(),
    entry_points={'console_scripts': ['sonar = sonar.cli:main']}
)

#FIXME: example configs, mappings, etc. should be part of the installable package
