from setuptools import setup, find_packages

setup(
    name="sonar",
    packages=find_packages(),
    entry_points={'console_scripts': ['sonar = sonar.main:main']},
    install_requires=[
        'click==7.0',
    ],
)
