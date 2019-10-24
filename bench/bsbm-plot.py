import xml.etree.ElementTree as ET
import matplotlib.pyplot as plt
from collections import defaultdict
from glob import glob
from numpy import array

def plot_y_per_x_per_plot(data, xlabel, ylabel, file, log=False):
    plt.figure(file)

    bar_width = 1 / (len(data) + 1)
    for i, (label, xys) in enumerate(sorted(data.items())):
        plt.bar(array(list(xys.keys())) + bar_width * (i + 1 - len(data) / 2), array(list(xys.values())), bar_width, label=label)

    plt.legend()
    plt.xlabel(xlabel)
    plt.ylabel(ylabel)
    if log:
        plt.yscale('log')
    plt.savefig(file)


# BSBM explore
aqet = defaultdict(dict)
for file in glob('bsbm.explore.*.xml'):
    run = file.replace('bsbm.explore.', '').replace('.xml', '')
    for query in ET.parse(file).getroot().find('queries').findall('query'):
        val =  float(query.find('aqet').text)
        if val > 0:
            aqet[run][int(query.attrib['nr'])] = val
plot_y_per_x_per_plot(aqet, 'query id', 'execution time (s)', 'bsbm.explore.svg')

# BSBM business intelligence
aqet = defaultdict(dict)
for file in glob('bsbm.businessIntelligence.*.xml'):
    run = file.replace('bsbm.businessIntelligence.', '').replace('.xml', '')
    for query in ET.parse(file).getroot().find('queries').findall('query'):
        val =  float(query.find('aqet').text)
        if val > 0:
            aqet[run][int(query.attrib['nr'])] = val
plot_y_per_x_per_plot(aqet, 'query id', 'execution time (s) - log scale', 'bsbm.businessIntelligence.svg', log=True)

plt.show()
