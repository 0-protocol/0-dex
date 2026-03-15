from setuptools import setup, find_packages

setup(
    name="zero-dex-lite",
    version="0.1.0",
    description="The Zero-Friction Python SDK for Agent-Native Trading on 0-dex",
    packages=find_packages(),
    install_requires=[
        "requests>=2.25.0",
        "eth-account>=0.8.0",
        "eth-abi>=5.0.0",
        "eth-utils>=2.0.0"
    ],
)
