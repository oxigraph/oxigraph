import xml.etree.ElementTree as ET
import matplotlib.pyplot as plt
from collections import defaultdict
from glob import glob


def plot_y_per_x_per_plot(data, xlabel, ylabel, file):
    plt.figure(file)
    for label, xys in data.items():
        plt.plot(list(xys.keys()), list(xys.values()), '.', label=label)
    plt.legend()
    plt.xlabel(xlabel)
    plt.ylabel(ylabel)
    # plt.yscale('log')
    plt.savefig(file)


# BSBM explore
aqet = defaultdict(dict)
for file in glob('bsbm.explore.*.xml'):
    run = file.replace('bsbm.explore.', '').replace('.xml', '')
    for query in ET.parse(file).getroot().find('queries').findall('query'):
        val =  float(query.find('aqet').text)
        if val > 0:
            aqet[run][int(query.attrib['nr'])] = val
plot_y_per_x_per_plot(aqet, 'query id', 'aqet', 'bsbm.explore.png')

# BSBM business intelligence
aqet = defaultdict(dict)
for file in glob('bsbm.businessIntelligence.*.xml'):
    run = file.replace('bsbm.businessIntelligence.', '').replace('.xml', '')
    for query in ET.parse(file).getroot().find('queries').findall('query'):
        val =  float(query.find('aqet').text)
        if val > 0:
            aqet[run][int(query.attrib['nr'])] = val
plot_y_per_x_per_plot(aqet, 'query id', 'aqet', 'bsbm.businessIntelligence.png')

plt.show()
